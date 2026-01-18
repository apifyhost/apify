import { useState } from 'react';
import {
  Button,
  Table,
  Space,
  Modal,
  Form,
  Input,
  Select,
  message,
  Tag,
  Popconfirm,
} from 'antd';
import { PlusOutlined, EditOutlined, DeleteOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import {
  useGetApisQuery,
  useCreateApiMutation,
  useDeleteApiMutation,
  useGetDataSourcesQuery,
  ApiConfig,
} from '@/services/api';
import dayjs from 'dayjs';

export const ApisPage = () => {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [form] = Form.useForm();
  const { data: apis = [], isLoading } = useGetApisQuery();
  const { data: dataSources = [] } = useGetDataSourcesQuery();
  const [createApi, { isLoading: isCreating }] = useCreateApiMutation();
  const [deleteApi] = useDeleteApiMutation();

  const handleCreate = () => {
    setIsModalOpen(true);
    form.resetFields();
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      await createApi(values).unwrap();
      message.success('API 配置创建成功');
      setIsModalOpen(false);
      form.resetFields();
    } catch (error) {
      message.error('创建失败');
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteApi(id).unwrap();
      message.success('删除成功');
    } catch (error) {
      message.error('删除失败');
    }
  };

  const columns: ColumnsType<ApiConfig> = [
    {
      title: 'API 名称',
      dataIndex: 'name',
      key: 'name',
      width: 200,
    },
    {
      title: '版本',
      dataIndex: 'version',
      key: 'version',
      width: 120,
      render: (version) => <Tag color="blue">{version}</Tag>,
    },
    {
      title: '数据源',
      dataIndex: 'datasource_name',
      key: 'datasource_name',
      width: 150,
    },
    {
      title: '表数量',
      key: 'schemas',
      width: 100,
      render: (_, record) => record.schemas?.length || 0,
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date) => dayjs(date).format('YYYY-MM-DD HH:mm:ss'),
    },
    {
      title: '操作',
      key: 'action',
      width: 150,
      fixed: 'right',
      render: (_, record) => (
        <Space size="middle">
          <Button
            type="link"
            icon={<EditOutlined />}
            onClick={() => message.info('编辑功能开发中')}
          >
            编辑
          </Button>
          <Popconfirm
            title="确定要删除这个 API 配置吗？"
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
          >
            <Button type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between' }}>
        <h1>API 配置管理</h1>
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
          新建 API
        </Button>
      </div>

      <Table
        columns={columns}
        dataSource={apis}
        rowKey="id"
        loading={isLoading}
        scroll={{ x: 1000 }}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条`,
        }}
      />

      <Modal
        title="创建 API 配置"
        open={isModalOpen}
        onOk={handleSubmit}
        onCancel={() => setIsModalOpen(false)}
        confirmLoading={isCreating}
        width={600}
      >
        <Form form={form} layout="vertical" style={{ marginTop: 24 }}>
          <Form.Item
            name="name"
            label="API 名称"
            rules={[{ required: true, message: '请输入 API 名称' }]}
          >
            <Input placeholder="例如: user-api" />
          </Form.Item>
          <Form.Item
            name="version"
            label="版本号"
            rules={[{ required: true, message: '请输入版本号' }]}
            initialValue="v1.0.0"
          >
            <Input placeholder="例如: v1.0.0" />
          </Form.Item>
          <Form.Item
            name="datasource_name"
            label="数据源"
            rules={[{ required: true, message: '请选择数据源' }]}
          >
            <Select placeholder="选择数据源">
              {dataSources.map((ds) => (
                <Select.Option key={ds.name} value={ds.name}>
                  {ds.name} ({ds.db_type})
                </Select.Option>
              ))}
            </Select>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
};
